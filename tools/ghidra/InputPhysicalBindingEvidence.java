import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.*;
public class InputPhysicalBindingEvidence extends GhidraScript {
  DecompInterface d = new DecompInterface();
  void dec(long x) throws Exception {
    Function f = getFunctionContaining(toAddr(x));
    println(String.format("\n=== DEC 0x%08X ===", x));
    if (f == null) { println("NO_FUNCTION"); return; }
    println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
    DecompileResults r = d.decompileFunction(f, 60, monitor);
    if (r.decompileCompleted() && r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    else println("ERR="+r.getErrorMessage());
  }
  void refs(long x) {
    println(String.format("\n=== REFS 0x%08X ===", x));
    ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(toAddr(x));
    while (it.hasNext()) {
      Reference r=it.next();
      Function f=getFunctionContaining(r.getFromAddress());
      println(r.getReferenceType()+" "+r.getFromAddress()+" FROM "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
    }
  }
  void dump(long x, int n) throws Exception {
    println(String.format("\n=== BYTES 0x%08X len=%d ===",x,n));
    Memory m=currentProgram.getMemory(); Address a=toAddr(x);
    byte[] b=new byte[n]; int got=m.getBytes(a,b);
    for(int i=0;i<got;i+=16){
      StringBuilder s=new StringBuilder(String.format("%08X: ",x+i));
      for(int j=0;j<16 && i+j<got;j++) s.append(String.format("%02X ",b[i+j]&0xff));
      println(s.toString());
    }
  }
  public void run() throws Exception {
    d.openProgram(currentProgram);
    long[] funcs={0x004486A0L,0x00508519L,0x00508625L,0x00507700L,0x0050772CL,0x00508C76L,0x005092B4L,0x00509396L};
    for(long x:funcs) dec(x);
    refs(0x007BE1B0L);
    dump(0x007BE180L,0x180);
    d.dispose();
  }
}