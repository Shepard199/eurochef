import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
public class CurrentAttackerBaseRefAssembly extends GhidraScript {
 @Override public void run() throws Exception {
  long[] refs={0x00435E4EL,0x00435EEEL,0x0043621EL,0x00444D48L,0x0044F0E6L,0x0044FF06L,0x00450560L,0x00450B74L,0x004512B5L,0x004512F6L,0x00455F81L,0x00455FA1L,0x0046AAF5L};
  Listing l=currentProgram.getListing();
  for(long raw:refs){
   Address a=toAddr(raw); Instruction hit=l.getInstructionContaining(a);
   println(String.format("\n=== REF 0x%08X in %s ===",raw,hit==null?"<none>":hit.toString()));
   if(hit==null)continue;
   Instruction cur=hit;
   for(int i=0;i<12 && cur.getPrevious()!=null;i++)cur=cur.getPrevious();
   for(int i=0;i<30 && cur!=null;i++,cur=cur.getNext())println(cur.getAddress()+"  "+cur);
  }
 }
}