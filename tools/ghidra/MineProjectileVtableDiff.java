import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;

public class MineProjectileVtableDiff extends GhidraScript {
    private long u32(long a) throws Exception { return Integer.toUnsignedLong(getInt(toAddr(a))); }
    @Override public void run() throws Exception {
        long base = 0x005DF2B0L;
        long mine = 0x005DF390L;
        println("XItemHandler_Projectile vtable=0x005DF2B0 vs XItemHandler_Mine vtable=0x005DF390");
        for (int i=0;i<64;i++) {
            long b=u32(base+i*4L), m=u32(mine+i*4L);
            if (b != m) println(String.format("slot +0x%02X base=0x%08X mine=0x%08X",i*4,b,m));
        }
        println(String.format("Mine descriptor ctor=0x%08X", u32(0x005DF250L + 0x0CL)));
    }
}
