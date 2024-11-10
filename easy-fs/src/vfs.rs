use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// Call a function over a disk inode to read it
    pub fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    pub fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    pub fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }
    ///
    pub fn linkat(&self,old_name:&str,new_name:&str)->isize{
        if old_name==new_name{
            return -1;
        }
        let mut fs = self.fs.lock();
        let inode_id=self.read_disk_inode(|disk_inode| {
            self.find_inode_id(old_name, disk_inode)
        });
        //let direntry=Directly::new(new_name,inode_id);
        //写到根那个目录下
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(new_name, inode_id.unwrap());
            root_inode.write_at(
            file_count * DIRENT_SZ,
            dirent.as_bytes(),
            &self.block_device,
            );
            //root_inode.nlinks+=1;
        });

        let (block_id,block_offset) = fs.get_disk_inode_pos(inode_id.unwrap());
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
        .lock()
        .modify(block_offset,|disk_inode:&mut DiskInode|{
            disk_inode.nlinks+=1;
            disk_inode.nlinks
        });
        return 0;
    }
    ///
    /*
    pub fn unlinkat(&self,_name:&str)->isize{
        let fs = self.fs.lock();
        log::debug!("find inode_id ok?");
        let inode_id=self.read_disk_inode(|disk_inode| {
            self.find_inode_id(_name, disk_inode)
        });
        if inode_id==None{
            return -1;
        }
        let (block_id,block_offset) = fs.get_disk_inode_pos(inode_id.unwrap());
        log::debug!("modify ok?");
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
        .lock()
        .modify(block_offset,|disk_inode:&mut DiskInode|{
            //self.modify_entry(_name,disk_inode);
            disk_inode.nlinks-=1;
            if disk_inode.nlinks==0{
                disk_inode.clear_size(&Arc::clone(&self.block_device));
            }
            disk_inode.nlinks
        });
        log::debug!("ha");
        return 0;
    }
     */
    pub fn unlinkat(&self, name: &str) -> isize{
        let fs = self.fs.lock();
        let inode_id = self.read_disk_inode(|disk_inode|{
            self.find_inode_id(name,disk_inode)
        });
        if let Some(i_id) = inode_id{
            let (block_id,block_offset) = fs.get_disk_inode_pos(i_id);
            get_block_cache(block_id as usize, Arc::clone(&self.block_device)).lock()
            .modify(block_offset,|disk_inode:&mut DiskInode|{
                disk_inode.nlinks -= 1;
                if disk_inode.nlinks == 0 {
                    disk_inode.clear_size(&Arc::clone(&self.block_device));
                }
                disk_inode.nlinks  
            });
            self.modify_disk_inode(|root_inode|{
                // let file_count = (root_inode.size as usize) / DIRENT_SZ;
                // let mut dirent = DirEntry::empty();
                // for i in 0..file_count{
                //     root_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device);
                //     if dirent.name() == name{
                //         for j in i..file_count - 1 {
                //             root_inode.read_at((j + 1) * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device);
                //             root_inode.write_at(j * DIRENT_SZ, dirent.as_bytes(), &self.block_device);
                //         }
                //         let new_size = (file_count - 1) * DIRENT_SZ;
                //         root_inode.size = new_size as u32;
                //         break;
                //     }
                // }
                self.modify_entry(name,root_inode);
            });
            return 0;
        };
        return  -1;
    }

    ///
    pub fn state(&self,inode_id: u64)->(u32,bool){
       // println!("here 5");
        let mut nlink:u32 = 0;
        let mut is = true;
        let fs = self.fs.lock();
       // println!("here 6");
        let (block_id,block_offset) = fs.get_disk_inode_pos(inode_id as u32);
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
        .lock()
        .modify(block_offset,|disk_inode:&mut DiskInode|{
            nlink = disk_inode.nlinks;
            is=disk_inode.is_file();
            0
        });
       // println!("here 7");
        (nlink,is)
    }
    ///
    pub fn get_inode_id_from_name(&self,name:&str)->u32{
        let inode_id=self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode)
        }); 
        inode_id.unwrap()
    }
    ///
    pub fn modify_entry(&self,name:&str,disk_inode: &mut DiskInode){
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();

        for i in 0..file_count{
            disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device);
            log::debug!("name()");
            if dirent.name() == name{
                for j in i..file_count - 1 {
                    disk_inode.read_at((j + 1) * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device);
                    disk_inode.write_at(j * DIRENT_SZ, dirent.as_bytes(), &self.block_device);
                }
                let new_size = (file_count - 1) * DIRENT_SZ;
                disk_inode.size = new_size as u32;
                break;
            }
            log::debug!("name()");
        }
        /* 
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(
                    DIRENT_SZ * i,
                    dirent.as_bytes_mut(),
                    &self.block_device,
                ),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                disk_inode.write_at(
                    DIRENT_SZ * i,
                    DirEntry::empty().as_bytes(),
                    &self.block_device,
                );
            }
        }
        */
    }
}
